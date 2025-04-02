use openai::chat::{
    ChatCompletionFunctionCall, ChatCompletionFunctionDefinition, ChatCompletionMessage,
    ChatCompletionMessageRole,
};
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::any::TypeId;
use std::sync::Arc;
use thiserror::Error;

type GenericCallableFn = Arc<dyn Fn(&str) -> Result<String, DispatchError>>;

#[derive(Clone)]
struct CallableFunction {
    name: String,
    func: GenericCallableFn,
}

impl CallableFunction {
    fn call(&self, args: &str) -> Result<String, DispatchError> {
        (self.func)(args)
    }
}

#[derive(Default)]
pub struct CallableFunctionList {
    functions: Vec<CallableFunction>,
    function_definitions: Vec<ChatCompletionFunctionDefinition>,
}

impl CallableFunctionList {
    /// Adds a function to the list of callable functions.
    pub fn add_function<F, A, R>(&mut self, name: &str, description: &str, function: F)
    where
        F: Fn(A) -> R + 'static,
        A: DeserializeOwned + JsonSchema + 'static,
        R: Serialize + 'static,
    {
        assert!(!self.function_definitions.iter().any(|e| e.name == name));

        let caller = move |args_str: &str| {
            let args_str = if TypeId::of::<()>() == TypeId::of::<A>() {
                "null"
            } else {
                args_str
            };
            let args: A = serde_json::from_str(args_str).map_err(DispatchError::Deserialize)?;
            let result: R = function(args);
            serde_json::to_string(&result).map_err(DispatchError::Serialize)
        };

        self.functions.push(CallableFunction {
            name: name.to_string(),
            func: Arc::new(caller),
        });

        let argument_schema = if TypeId::of::<()>() == TypeId::of::<A>() {
            Value::Null
        } else {
            let mut schema = schema_for!(A);
            schema.meta_schema = None;
            schema.schema.metadata.as_mut().unwrap().title = None;
            serde_json::to_value(&schema).unwrap()
        };

        let definition = ChatCompletionFunctionDefinition {
            name: name.to_string(),
            description: Some(description.to_string()),
            parameters: Some(argument_schema),
        };
        log::debug!("Adding function: {definition:?}");
        self.function_definitions.push(definition);
    }

    /// Returns the function definitions.
    pub fn function_definitions(&self) -> Vec<ChatCompletionFunctionDefinition> {
        self.function_definitions.clone()
    }

    /// Dispatches the function call to the appropriate function.
    pub fn dispatch(
        &self,
        call: &ChatCompletionFunctionCall,
    ) -> Result<ChatCompletionMessage, DispatchError> {
        let function = self
            .functions
            .iter()
            .find(|f| f.name == call.name)
            .ok_or(DispatchError::FunctionNotFound)?;

        let output = function.call(&call.arguments)?;
        let message = ChatCompletionMessage {
            role: ChatCompletionMessageRole::Function,
            content: Some(output),
            name: Some(call.name.clone()),
            ..Default::default()
        };
        Ok(message)
    }
}

#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("Function not found")]
    FunctionNotFound,
    #[error("Failed to deserialize function argument")]
    Deserialize(#[source] serde_json::Error),
    #[error("Failed to serialize function result")]
    Serialize(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_type_args() {
        let mut list = CallableFunctionList::default();

        #[derive(Serialize)]
        struct FuncResult {
            message: String,
        }
        list.add_function("unit_test", "unit test function", |_: ()| FuncResult {
            message: "Hello".to_string(),
        });

        let message = list
            .dispatch(&ChatCompletionFunctionCall {
                name: "unit_test".to_string(),
                arguments: "{}".to_string(),
            })
            .unwrap();
        assert_eq!(message.content.unwrap(), r#"{"message":"Hello"}"#);
    }
}
