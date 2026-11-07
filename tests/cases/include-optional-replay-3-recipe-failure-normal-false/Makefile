all: requested
requested: shared0
-include optional.mk
optional.mk: shared0
shared0: shared1
shared1: shared2
shared2: shared3
shared3: ; @printf "fail\n"; exit 7
