$(shell mkdir -p pool/nested; printf 'result\n' > result; touch -d @1000000200 result)
VPATH := pool
all: result
result: nested/part
	@printf 'consumer [%s][%s][%s]\n' '$<' '$?' '$^'
