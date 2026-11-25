from os import utime

utime('input', (1_600_000_000, 1_600_000_000))
utime('parent', (1_600_000_001, 1_600_000_001))
utime('other', (1_600_000_002, 1_600_000_002))
