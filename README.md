# `riir`

> Use AI powers to rewrite code in blazingly fast language🦀

## Getting started

Install Rust and Cargo if you haven't already. You can find the installation
instructions [here](https://www.rust-lang.org/tools/install).

Install Podman.

Clone this repository:

```bash
git clone https://github.com/Disasm/riir.git
cd riir
```

Create a new file called `.env` and add your OpenAI API key to it.
You can get your OpenAI API key from [here](https://platform.openai.com/signup).
See the example in the `.env.example` file.

Create a directory for the project and run the tool:

```bash
mkdir -p output
cargo run --release -- <source_project> output
```

Sit back and relax while the tool spends money from your OpenAI account to rewrite the code for you.
