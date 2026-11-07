$(shell mkdir -p pool; printf 'result\n' > result; touch -d @1000000200 result; printf 'source\n' > pool/part; touch -d @1000000100 pool/part)
vpath % ././pool
all: result
result: part
	@printf 'consumer [%s][%s][%s]\n' '$<' '$?' '$^'
