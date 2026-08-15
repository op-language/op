-- ftdetect for Op source files
-- Register the .op extension so Neovim uses the op filetype.

vim.api.nvim_create_autocmd({ "BufNewFile", "BufRead" }, {
  pattern = "*.op",
  callback = function()
    vim.bo.filetype = "op"
  end,
})