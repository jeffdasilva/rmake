import os
for name, value in os.environ.items():
    if name in ('.SHELLSTATUS', 'ANCHOR_3190', 'ANCHOR_4413'):
        print(f'{name}|{value}')
