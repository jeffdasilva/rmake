VPATH = cache
$(shell mkdir -p cache; touch -d @1600000002 cache/prior.out; touch -d @1600000004 sink; touch -d @1600000000 seed; touch -d @1600000000 other)
.SECONDEXPANSION:
prior.out:: seed ; @echo BUILD-FIRST
prior.out:: other ; @echo BUILD-SECOND
sink: prior.out ; @echo 'SINK:$?:$^'
