X = $(file >>seen,base)$(eval parent: X := changed)outer
parent: X += $(file >>seen,parent-old)parent-tail
child: X += $(file >>seen,child)child-tail
.PHONY: parent child
parent: child
parent: ; $(info parent=[$(X)])@:
child: ; $(info first=[$(X)] second=[$(X)])@:
