use crate::function::CallableFunctionList;
use crate::project::{Project, ReadFileArgs, WriteFileArgs};
use argh::FromArgs;
use dotenvy::dotenv;
use log::{debug, error, info};
use openai::Credentials;
use openai::chat::{ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

mod function;
mod project;

#[derive(FromArgs)]
/// a command line interface for a large language model
struct Args {
    /// path to the source project directory
    #[argh(positional)]
    source: PathBuf,

    /// path to the destination project directory
    #[argh(positional)]
    destination: PathBuf,
}

fn dump_message(message: &ChatCompletionMessage) {
    let role = message.role;
    if let Some(text) = &message.content {
        if [
            ChatCompletionMessageRole::System,
            ChatCompletionMessageRole::User,
            ChatCompletionMessageRole::Assistant,
        ]
        .contains(&role)
        {
            println!("==== {role:#?} ====\n{text}\n");
        }
    }
    debug!("{role:#?}: {message:#?}");
}

struct Chat {
    model: String,
    credentials: Credentials,
    messages: Vec<ChatCompletionMessage>,
    functions: CallableFunctionList,
}

impl Chat {
    fn new(model: String, credentials: Credentials) -> Self {
        Chat {
            model,
            credentials,
            messages: vec![],
            functions: Default::default(),
        }
    }

    fn from_env() -> Self {
        let model = env::var("MODEL").unwrap();
        let credentials = Credentials::from_env();
        Chat::new(model, credentials)
    }

    async fn send_message(&mut self, message: &str) {
        let chat_message = ChatCompletionMessage {
            role: ChatCompletionMessageRole::User,
            content: Some(message.to_string()),
            ..Default::default()
        };

        dump_message(&chat_message);
        self.messages.push(chat_message);

        self.execute().await;
    }

    async fn execute(&mut self) {
        loop {
            let chat_completion = ChatCompletion::builder(&self.model, self.messages.clone())
                .credentials(self.credentials.clone())
                .functions(self.functions.function_definitions())
                .create()
                .await
                .unwrap();

            let returned_message = chat_completion.choices.first().unwrap().message.clone();
            self.messages.push(returned_message.clone());

            dump_message(&returned_message);

            if let Some(call) = returned_message.function_call.as_ref() {
                let message = self.functions.dispatch(call).unwrap();
                dump_message(&message);
                self.messages.push(message);
            } else {
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Args = argh::from_env();

    if !args.source.is_dir() {
        error!("The source project directory does not exist.");
        return;
    }
    if !args.destination.is_dir() {
        error!("The destination project directory does not exist.");
        return;
    }
    let source_project = Arc::new(Project::new(args.source));
    let destination_project = Arc::new(Project::new(args.destination));

    // Make sure you have a file named `.env` with the `OPENAI_KEY` environment variable defined!
    dotenv().unwrap();

    let mut chat = Chat::from_env();

    let project = source_project.clone();
    chat.functions.add_function(
        "src_list_files",
        "List all files in the source project directory.",
        move |_: ()| project.list_contents(),
    );

    let project = source_project.clone();
    chat.functions.add_function(
        "src_read_file",
        "Reads the contents of a file in the source project directory.",
        move |args: ReadFileArgs| project.read_file(&args.path),
    );

    let project = destination_project.clone();
    chat.functions.add_function(
        "dst_list_files",
        "List all files in the destination project directory.",
        move |_: ()| project.list_contents(),
    );

    let project = destination_project.clone();
    chat.functions.add_function(
        "dst_read_file",
        "Reads the contents of a file in the destination project directory.",
        move |args: ReadFileArgs| project.read_file(&args.path),
    );

    let project = destination_project.clone();
    chat.functions.add_function(
        "dst_write_file",
        "Saves the contents to a file in the destination project directory.",
        move |args: WriteFileArgs| project.write_file(&args.path, &args.contents),
    );

    let system_prompt = "\
        You are a large language model that is capable of converting project source code to Rust source code. \
        You have access to two project directories: the source project directory if read-only and contains the source files of the original project. \
        The destination project directory is initially empty and should be populated with project files in Rust language. \
        When you propose an action or a change to the source code, execute this action or change right away.\
    ".to_string();
    let system_message = ChatCompletionMessage {
        role: ChatCompletionMessageRole::System,
        content: Some(system_prompt),
        ..Default::default()
    };
    dump_message(&system_message);
    chat.messages = vec![system_message];

    chat.send_message("Please analyze the project in the source directory, but don't make any changes at this point.").await;

    let mut message = "Now create Rust project in the destination project directory so that it matches the implementation in the source project directory.".to_string();
    loop {
        chat.send_message(&message).await;
        if destination_project.is_dirty() {
            destination_project.clear_dirty();

            let errors = destination_project.run_cargo_check();
            if let Some(errors) = errors {
                message = "Apparently there are some problems with the code. Please correct them. Here is the `cargo check` output:\n".to_string() + &errors;
                continue;
            }
        }
        break;
    }
}
