import os
names = {'.SHELLSTATUS', 'ANCHOR_3190', 'ANCHOR_4413'}
for name, value in os.environ.items():
    if name in names:
        print(f'{name}|{value}')
