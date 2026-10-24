$(shell touch -d @1600000001 input; touch -d @1600000020 output)
vpath %.a store
output: lib.a(one.o);@printf "output rebuilt\n"
lib.a(one.o): input;@cp replacement.a lib.a; printf "member rebuilt\n"
