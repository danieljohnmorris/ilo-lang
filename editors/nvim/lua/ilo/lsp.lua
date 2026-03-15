local M = {}

--- Configure the ilo LSP client.
---
--- @param opts? table Options:
---   - cmd: string|string[]  Path to the ilo binary (default: "ilo")
---   - root_markers: string[]  Files that mark a project root (default: {".git"})
function M.setup(opts)
  opts = opts or {}
  local cmd = opts.cmd or "ilo"
  local root_markers = opts.root_markers or { ".git" }

  vim.api.nvim_create_autocmd("FileType", {
    pattern = "ilo",
    callback = function(ev)
      vim.lsp.start({
        name = "ilo",
        cmd = type(cmd) == "string" and { cmd, "lsp" } or cmd,
        root_dir = vim.fs.root(ev.buf, root_markers),
        capabilities = opts.capabilities,
        on_attach = opts.on_attach,
      })
    end,
  })
end

return M
