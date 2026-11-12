.RECIPEPREFIX := >
%.a %.b &: %.in ; @printf pattern-marker
.RECIPEPREFIX := !
all: ; @:
