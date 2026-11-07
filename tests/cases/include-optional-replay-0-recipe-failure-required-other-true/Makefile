all: requested
requested: shared0
-include optional.mk
optional.mk: shared0
include required.mk
required.mk: shared0
shared0: ; @printf "fail\n"; exit 7
