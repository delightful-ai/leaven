// TODO: DO NOT EXPOSE THIS
use internal_baml_core::ir::FunctionNode;

use super::{stream_type_to_go, type_to_go};
use crate::{functions::FunctionGo, package::CurrentRenderPackage};

pub fn ir_function_to_go(function: &FunctionNode, pkg: &CurrentRenderPackage) -> FunctionGo {
    FunctionGo {
        documentation: None,
        name: function.elem.name().to_string(),
        args: function
            .elem
            .inputs()
            .iter()
            .map(|(name, field_type)| {
                (
                    name.clone(),
                    type_to_go(
                        &field_type.to_non_streaming_type(pkg.lookup()),
                        pkg.lookup(),
                    ),
                )
            })
            .collect(),
        return_type: type_to_go(
            &function.elem.output().to_non_streaming_type(pkg.lookup()),
            pkg.lookup(),
        ),
        stream_return_type: stream_type_to_go(
            &function.elem.output().to_streaming_type(pkg.lookup()),
            pkg.lookup(),
        ),
    }
}
