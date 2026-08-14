-- ftdetect for HLA source files
-- Register the .hla extension so Neovim uses the hla filetype.

vim.api.nvim_create_autocmd({ "BufNewFile", "BufRead" }, {
  pattern = "*.hla",
  callback = function()
    vim.bo.filetype = "hla"
  end,
})