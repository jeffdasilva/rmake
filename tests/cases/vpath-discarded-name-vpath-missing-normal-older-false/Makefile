$(shell mkdir -p depot; printf 'initial\n' > seed; touch -d @1000000300 seed; printf 'initial\n' > depot/mid.unit; touch -d @1000000100 depot/mid.unit; printf 'initial\n' > product; touch -d @1000000090 product)
VPATH = depot
all: product
product:  mid.unit
	@printf 'consumer [%s][%s][%s]\n' '$^' '$?' '$|'
mid.unit: seed
	@printf 'producer [%s][%s]\n' '$@' '$^'
