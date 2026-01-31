# Installation

## From crates.io (recommended)

```bash
cargo install fmm
```

## Pre-built binaries

Download pre-built binaries from [GitHub Releases](https://github.com/srobinson/fmm/releases):

| Platform | Architecture | Download |
|----------|-------------|----------|
| macOS | Apple Silicon (M1+) | `fmm-aarch64-apple-darwin.tar.gz` |
| macOS | Intel | `fmm-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 | `fmm-x86_64-unknown-linux-gnu.tar.gz` |
| Windows | x86_64 | `fmm-x86_64-pc-windows-msvc.zip` |

```bash
# Example: macOS Apple Silicon
curl -fsSL https://github.com/srobinson/fmm/releases/latest/download/fmm-aarch64-apple-darwin.tar.gz | tar xz
sudo mv fmm /usr/local/bin/
```

## From source

```bash
git clone https://github.com/srobinson/fmm
cd fmm
cargo install --path .
```

## Verify installation

```bash
fmm --version
```

## Shell completions

Generate tab-completion scripts for your shell:

```bash
# Bash
fmm completions bash > ~/.local/share/bash-completion/completions/fmm

# Zsh
fmm completions zsh > ~/.zfunc/_fmm

# Fish
fmm completions fish > ~/.config/fish/completions/fmm.fish

# PowerShell
fmm completions powershell > _fmm.ps1
```
