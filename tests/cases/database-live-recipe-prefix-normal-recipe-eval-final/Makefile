.PHONY: scan
scan:
	+@$(MAKE) -rR --no-print-directory -f rules.mk -p all > database.txt
	@awk -f select.awk database.txt
	@rm database.txt
