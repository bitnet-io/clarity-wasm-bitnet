use std::cell::OnceCell;

use crate::wasm_generator::{
    clar2wasm_ty, drop_value, ArgumentsExt, GeneratorError, WasmGenerator,
};
use clarity::vm::{
    types::{signatures::CallableSubtype, SequenceSubtype, StringSubtype, TypeSignature},
    ClarityName, SymbolicExpression,
};
use walrus::{
    ir::{BinaryOp, IfElse, UnaryOp},
    InstrSeqBuilder, LocalId, ValType,
};

use super::Word;

#[derive(Debug)]
pub struct IsEq;

impl Word for IsEq {
    fn name(&self) -> ClarityName {
        "is-eq".into()
    }

    fn traverse(
        &self,
        generator: &mut WasmGenerator,
        builder: &mut walrus::InstrSeqBuilder,
        _expr: &SymbolicExpression,
        args: &[SymbolicExpression],
    ) -> Result<(), GeneratorError> {
        // Traverse the first operand pushing it onto the stack
        let first_op = args.get_expr(0)?;
        generator.traverse_expr(builder, first_op)?;

        // Save the first_op to a local to be further used.
        // This allows to use the first_op value without
        // traversing again the expression.
        let ty = generator
            .get_expr_type(first_op)
            .expect("is-eq value expression must be typed")
            .clone();
        let val_locals = generator.save_to_locals(builder, &ty, true);

        // Explicitly set to true.
        // Shortcut for a case with only one operand.
        builder.i32_const(1);

        let mut all_correct_type = true;
        let mut nth_locals = Vec::with_capacity(wasm_types.len());
        // Loop through remainder operands, if the case.
        for operand in args.iter().skip(1) {
            // push the new operand on the stack
            generator.traverse_expr(builder, operand)?;

            // check if types are identical:
            // if yes, we go with equality checks,
            // otherwise it's automatically false

            let operand_ty = generator
                .get_expr_type(operand)
                .expect("is-eq value expression must be typed");

            if all_correct_type && &ty == operand_ty {
                // insert the new operand into locals
                for local_ty in wasm_types.iter().rev() {
                    let local = generator.module.locals.add(*local_ty);
                    nth_locals.push(local);
                    builder.local_set(local);
                }
                nth_locals.reverse();

                // check equality
                wasm_equal(&ty, generator, builder, &val_locals, &nth_locals)?;

                nth_locals.clear();
            } else {
                all_correct_type &= false;
                drop_value(builder, operand_ty);
                builder.i32_const(0);
            }
            // Do an "and" operation with the result from the previous function call.
            builder.binop(BinaryOp::I32And);
        }

        Ok(())
    }
}

fn wasm_equal(
    ty: &TypeSignature,
    generator: &mut WasmGenerator,
    builder: &mut InstrSeqBuilder,
    first_op: &[LocalId],
    nth_op: &[LocalId],
) -> Result<(), GeneratorError> {
    match dbg!(ty) {
        // we should never compare NoType
        TypeSignature::NoType => {
            builder.unreachable();
            Ok(())
        }
        // is-eq-int function can be reused to both int and uint types.
        TypeSignature::IntType | TypeSignature::UIntType => {
            wasm_equal_int128(generator, builder, first_op, nth_op)
        }
        // is-eq-bytes function can be used for types with (offset, length)
        TypeSignature::SequenceType(SequenceSubtype::BufferType(_))
        | TypeSignature::SequenceType(SequenceSubtype::StringType(StringSubtype::ASCII(_)))
        | TypeSignature::PrincipalType
        | TypeSignature::CallableType(CallableSubtype::Principal(_)) => {
            wasm_equal_bytes(generator, builder, first_op, nth_op)
        }
        TypeSignature::OptionalType(some_ty) => {
            wasm_equal_optional(generator, builder, first_op, nth_op, some_ty)
        }
        TypeSignature::ResponseType(ok_err_ty) => wasm_equal_response(
            generator,
            builder,
            first_op,
            nth_op,
            &ok_err_ty.0,
            &ok_err_ty.1,
        ),
        _ => Err(GeneratorError::NotImplemented),
    }
}

fn wasm_equal_int128(
    generator: &mut WasmGenerator,
    builder: &mut InstrSeqBuilder,
    first_op: &[LocalId],
    nth_op: &[LocalId],
) -> Result<(), GeneratorError> {
    // Get first operand from the local and put it onto stack.
    for val in first_op {
        builder.local_get(*val);
    }

    // Get second operand from the local and put it onto stack.
    for val in nth_op {
        builder.local_get(*val);
    }

    // Call the function with the operands on the stack.
    let func = OnceCell::new();
    builder.call(*func.get_or_init(|| generator.func_by_name("stdlib.is-eq-int")));

    Ok(())
}

fn wasm_equal_bytes(
    generator: &mut WasmGenerator,
    builder: &mut InstrSeqBuilder,
    first_op: &[LocalId],
    nth_op: &[LocalId],
) -> Result<(), GeneratorError> {
    // Get first operand from the local and put it onto stack.
    for val in first_op {
        builder.local_get(*val);
    }

    // Get second operand from the local and put it onto stack.
    for val in nth_op {
        builder.local_get(*val);
    }

    // Call the function with the operands on the stack.
    let func = OnceCell::new();
    builder.call(*func.get_or_init(|| generator.func_by_name("stdlib.is-eq-bytes")));

    Ok(())
}

fn wasm_equal_optional(
    generator: &mut WasmGenerator,
    builder: &mut InstrSeqBuilder,
    first_op: &[LocalId],
    nth_op: &[LocalId],
    some_ty: &TypeSignature,
) -> Result<(), GeneratorError> {
    let Some((first_variant, first_inner)) = first_op.split_first() else {
        return Err(GeneratorError::InternalError(
            "Optional operand should have at least one argument".into(),
        ));
    };
    let Some((nth_variant, nth_inner)) = nth_op.split_first() else {
        return Err(GeneratorError::InternalError(
            "Optional operand should have at least one argument".into(),
        ));
    };

    // check if we have (some x, some x) or (none, none)
    builder
        .local_get(*first_variant)
        .local_get(*nth_variant)
        .binop(BinaryOp::I32Eq);

    // if both operands are identical,
    // [then]: we check if we have a `none` or if the `some` inner_type are equal
    // [else]: we push "false" on the stack
    let then_id = {
        let mut then = builder.dangling_instr_seq(ValType::I32);
        // is none ?
        then.local_get(*first_variant).unop(UnaryOp::I32Eqz);
        // is some inner equal ?
        wasm_equal(some_ty, generator, &mut then, first_inner, nth_inner)?; // is some arguments equal ?
        then.binop(BinaryOp::I32Or);
        then.id()
    };

    let else_id = {
        let mut else_ = builder.dangling_instr_seq(ValType::I32);
        else_.i32_const(0);
        else_.id()
    };

    builder.instr(IfElse {
        consequent: then_id,
        alternative: else_id,
    });

    Ok(())
}

fn wasm_equal_response(
    generator: &mut WasmGenerator,
    builder: &mut InstrSeqBuilder,
    first_op: &[LocalId],
    nth_op: &[LocalId],
    ok_ty: &TypeSignature,
    err_ty: &TypeSignature,
) -> Result<(), GeneratorError> {
    let split_ok_err_idx = dbg!(clar2wasm_ty(ok_ty)).len();
    let Some((first_variant, first_ok, first_err)) =
        first_op.split_first().map(|(variant, rest)| {
            let (ok, err) = dbg!(rest.split_at(split_ok_err_idx));
            (variant, ok, err)
        })
    else {
        return Err(GeneratorError::InternalError(
            "Response operand should have at least one argument".into(),
        ));
    };
    let Some((nth_variant, nth_ok, nth_err)) = nth_op.split_first().map(|(variant, rest)| {
        let (ok, err) = dbg!(rest.split_at(split_ok_err_idx));
        (variant, ok, err)
    }) else {
        return Err(GeneratorError::InternalError(
            "Response operand should have at least one argument".into(),
        ));
    };

    // We will have a three branch if:
    // [ok] is the (ok, ok) case, we have to compare if both ok values are identical
    // [err] is the (err, err) case, we have to compare if both err values are identical
    // [else] is the (ok, err) or (err, ok) case, it is directly false

    let ok_id = {
        let mut ok_case = builder.dangling_instr_seq(ValType::I32);
        wasm_equal(ok_ty, generator, &mut ok_case, first_ok, nth_ok)?;
        ok_case.id()
    };

    let err_id = {
        let mut err_case = builder.dangling_instr_seq(ValType::I32);
        wasm_equal(err_ty, generator, &mut err_case, first_err, nth_err)?;
        err_case.id()
    };

    let else_id = {
        let mut else_ = builder.dangling_instr_seq(ValType::I32);
        else_.i32_const(0);
        else_.id()
    };

    // inner if is checking if both are err (consequent) or ok (alternative)
    let inner_if_id = {
        let mut inner_if = builder.dangling_instr_seq(ValType::I32);
        inner_if
            .local_get(*first_variant)
            // 0 is err
            .unop(UnaryOp::I32Eqz)
            .instr(IfElse {
                consequent: err_id,
                alternative: ok_id,
            });
        inner_if.id()
    };

    // outer if checks if both variants are identical (consequent) or not (alternative)
    builder
        .local_get(*first_variant)
        .local_get(*nth_variant)
        .binop(BinaryOp::I32Eq)
        .instr(IfElse {
            consequent: inner_if_id,
            alternative: else_id,
        });

    Ok(())
}
