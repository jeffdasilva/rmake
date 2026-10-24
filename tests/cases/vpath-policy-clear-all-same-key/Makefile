vpath %.src first
vpath %.src second
vpath %.src
VPATH=last
all: file.src
	@printf "first=[%s] all=[%s]\n" "$<" "$^"
