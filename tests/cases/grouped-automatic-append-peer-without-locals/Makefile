all: a b
a: X=A
b: X=B
a b &:;@$(eval b: @ += $$(file >>seen,from-$$@)tail)$(info effect=[$(wildcard seen)])$(info active=[$@|$(@)|$(value @)|$(origin @)|$(flavor @)])echo $(X);touch a b
