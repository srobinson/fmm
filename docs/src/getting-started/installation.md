# Installation

## From crates.io

```bash
cargo install fmm
```

## From source

```bash
git clone https://github.com/mdcontext/fmm
cd fmm
cargo install --path .
```

## Verify installation

```bash
fmm --version
```

## Shell completions

Generate completions for your shell:

```bash
# Bash
fmm completions bash > ~/.local/share/bash-completion/completions/fmm

# Zsh
fmm completions zsh > ~/.zfunc/_fmm

# Fish
fmm completions fish > ~/.config/fish/completions/fmm.fish
```
