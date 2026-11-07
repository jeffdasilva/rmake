$(shell printf 'initial\n' > app.out; touch -d @1000000100 app.out; printf 'initial\n' > app.part; touch -d @1000000090 app.part)
all: app.out
%.out:: %.part
	@printf 'output [%s]\n' '$^'
	@touch '$@'
%.part: ; @touch '$@'
spare: app.part
