# ilo.nvim — Neovim plugin for ilo

Syntax highlighting, filetype detection, and LSP client configuration for the [ilo programming language](https://ilo-lang.ai).

## Features

- Filetype detection for `.ilo` files
- Syntax highlighting (keywords, types, builtins, operators, comments, strings, numbers)
- Filetype-local settings (comment string, 2-space indent)
- LSP client wiring for `ilo lsp`

## Installation

### lazy.nvim

```lua
{
  "ilo-lang/ilo",
  -- point at the nvim subdirectory
  dir = vim.fn.expand("~/.local/share/nvim/site/pack/ilo/start/ilo-nvim"),
  ft = "ilo",
  config = function()
    require("ilo.lsp").setup()
  end,
}
```

Or install from the GitHub repo root with a subdirectory path:

```lua
{
  "ilo-lang/ilo",
  branch = "main",
  -- lazy.nvim doesn't natively support subdirs; clone manually (see below)
}
```

Because this plugin lives inside `editors/nvim/` of the main ilo repo, the simplest approach is to symlink or copy the directory:

```sh
# Symlink into your Neovim runtime path
ln -s /path/to/ilo/editors/nvim ~/.local/share/nvim/site/pack/ilo/start/ilo-nvim
```

### packer.nvim

```lua
use {
  "~/.local/share/nvim/site/pack/ilo/start/ilo-nvim",
  ft = "ilo",
  config = function()
    require("ilo.lsp").setup()
  end,
}
```

### Manual

Copy or symlink `editors/nvim/` to a directory on your `runtimepath`:

```sh
cp -r editors/nvim ~/.local/share/nvim/site/pack/ilo/start/ilo-nvim
# or
ln -s $(pwd)/editors/nvim ~/.local/share/nvim/site/pack/ilo/start/ilo-nvim
```

## LSP setup

### Without nvim-lspconfig (built-in `vim.lsp.start`)

The plugin ships a small wrapper around `vim.lsp.start`. Call `setup()` once in your config:

```lua
require("ilo.lsp").setup()
```

Options:

```lua
require("ilo.lsp").setup({
  -- Path to the ilo binary (default: "ilo", must be on $PATH)
  cmd = "ilo",
  -- Or provide the full command table:
  -- cmd = { "/usr/local/bin/ilo", "lsp" },

  -- Files that mark a project root (default: { ".git" })
  root_markers = { ".git", "*.ilo" },

  -- Optional: pass nvim-cmp or blink.nvim capabilities
  capabilities = require("cmp_nvim_lsp").default_capabilities(),

  -- Optional: on_attach callback
  on_attach = function(client, bufnr)
    -- your keymaps here
  end,
})
```

### With nvim-lspconfig (if a config is added in future)

```lua
require("lspconfig").ilo.setup({
  cmd = { "ilo", "lsp" },
  filetypes = { "ilo" },
  root_dir = require("lspconfig.util").root_pattern(".git"),
})
```

## Example keybindings

Add these inside an `on_attach` callback or a `LspAttach` autocmd:

```lua
vim.api.nvim_create_autocmd("LspAttach", {
  callback = function(ev)
    local buf = ev.buf
    local opts = { buffer = buf, silent = true }
    vim.keymap.set("n", "K",          vim.lsp.buf.hover,           opts)
    vim.keymap.set("n", "gd",         vim.lsp.buf.definition,      opts)
    vim.keymap.set("n", "gr",         vim.lsp.buf.references,      opts)
    vim.keymap.set("n", "<leader>rn", vim.lsp.buf.rename,          opts)
    vim.keymap.set("n", "<leader>ca", vim.lsp.buf.code_action,     opts)
    vim.keymap.set("n", "[d",         vim.diagnostic.goto_prev,    opts)
    vim.keymap.set("n", "]d",         vim.diagnostic.goto_next,    opts)
  end,
})
```

## Screenshot

<!-- TODO: add screenshot -->

## Language overview

ilo uses **prefix notation** — the operator comes first:

```ilo
-- Function: double a number
dbl x:n>n;*x 2

-- Guard (early return if condition is true)
cls sp:n>t;>=sp 1000 "gold";>=sp 500 "silver";"bronze"

-- While loop
wh-sum>n;i=0;s=0;wh <i 5{i=+i 1;s=+s i};+s 0

-- Foreach loop
range-sum>n;s=0;@i 0..5{s=+s i};+s 0

-- Record type
type point{x:n;y:n}
make-pt>n;p=pt x:3 y:4;p.x
```

See the full [language spec](https://github.com/ilo-lang/ilo/blob/main/SPEC.md) for details.
