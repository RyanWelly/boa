use crate::{
    builtins::{iterable::IteratorRecord, object::for_in_iterator::ForInIterator},
    js_string,
    vm::opcode::{Operation, VaryingOperand},
    Context, JsResult, JsValue,
};

/// `CreateForInIterator` implements the Opcode Operation for `Opcode::CreateForInIterator`
///
/// Operation:
///  - Creates a new `ForInIterator` for the provided object.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CreateForInIterator;

impl CreateForInIterator {
    #[inline(always)]
    pub(crate) fn operation(value: VaryingOperand, context: &mut Context) -> JsResult<()> {
        let object = context.vm.get_register(value.into()).clone();
        let object = object.to_object(context)?;
        let iterator = ForInIterator::create_for_in_iterator(JsValue::new(object), context);
        let next_method = iterator
            .get(js_string!("next"), context)
            .expect("ForInIterator must have a `next` method");

        context
            .vm
            .frame_mut()
            .iterators
            .push(IteratorRecord::new(iterator, next_method));

        Ok(())
    }
}

impl Operation for CreateForInIterator {
    const NAME: &'static str = "CreateForInIterator";
    const INSTRUCTION: &'static str = "INST - CreateForInIterator";
    const COST: u8 = 4;
}
