.RECIPEPREFIX := >
all: first second
> @printf top-marker
first: ; @printf first-marker
.RECIPEPREFIX := !
second: ; @printf second-marker
