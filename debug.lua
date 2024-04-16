local id = nil
local cur_buffer = nil


local function attach_lsp(args)
  if id == nil then
    return
  end

  vim.lsp.buf_attach_client(args.buffer, id);
  cur_buffer = args.buffer
end

vim.api.nvim_create_autocmd("BufNew", {
  callback = attach_lsp
});

vim.api.nvim_create_autocmd("BufEnter", {
  callback = attach_lsp,
});

local function start_lsp()
  if id ~= nil then
    return
  end
  id = vim.lsp.start({
    name = 'IEF LSP',
    cmd = { 'cargo', 'run' },
    root_dir = vim.loop.cwd(),
  })
end

local function stop_lsp()
  if id == nil then
    return
  end
  if cur_buffer ~= nil then
    vim.lsp.buf_attach_client(cur_buffer, id)
    cur_buffer = nil
  end
  vim.lsp.stop_client(id)
  id = nil
end

function Restart_LSP()
  stop_lsp()
  print("stop_lsp")
  start_lsp()
end

start_lsp()
