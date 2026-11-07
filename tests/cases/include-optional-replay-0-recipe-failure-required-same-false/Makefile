all: requested
requested: shared0
-include optional.mk
optional.mk: shared0
include optional.mk
shared0: ; @printf "fail\n"; exit 7
