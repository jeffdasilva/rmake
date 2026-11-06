$(shell mkdir -p pool; printf 'result\n' > result; touch -d @1000000200 result)
VPATH := ././pool
all: result
result: part
	@printf 'consumer [%s][%s][%s]\n' '$<' '$?' '$^'
