all:
	+@$(MAKE) -rR --no-print-directory -f commands.mk -np build > database.txt
	@awk '/^printf / && !seen { echo = 2; seen = 1 } echo { print; --echo } /^#  recipe to execute/ { raw = 1 } raw && NF { print } raw && !NF { exit }' database.txt
	@rm -f database.txt
