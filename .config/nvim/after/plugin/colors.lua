function ColorMyPencils(color)
	-- provide default color scheme
	color = color or "rose-pine"
	vim.cmd.colorscheme(color)

	-- set background colors as transparent (not currently working on Arch)
	-- vim.api.nvim_set_hl(0, "Normal", { bg = "none" })
	-- vim.api.nvim_set_hl(0, "NormalFloat", { bg = "none" })
end

ColorMyPencils()
