.RECIPEPREFIX := >
%.o:: %.in ; @printf pattern-marker
.RECIPEPREFIX := !
all: ; @:
