$(shell mkdir -p pool/nested; printf 'result\n' > result; touch -d @1000000200 result; printf 'source\n' > pool/nested/part; touch -d @1000000100 pool/nested/part)
VPATH := ./pool
all: result
result: nested/part
	@printf 'consumer [%s][%s][%s]\n' '$<' '$?' '$^'
