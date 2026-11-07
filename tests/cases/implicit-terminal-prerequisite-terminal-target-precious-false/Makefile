$(shell printf 'initial\n' > app.out; touch -d @1000000100 app.out)
all: app.out
%.out:: %.part
	@printf 'output [%s]\n' '$^'
	@touch '$@'
%.part: ; @touch '$@'
.PRECIOUS: app.part
