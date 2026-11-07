all: app.out
%.out:: %.part
	@printf 'output [%s]\n' '$^'
	@touch '$@'
%.part: ; @touch '$@'
spare: app.part
