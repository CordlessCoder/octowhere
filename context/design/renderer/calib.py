from lib import *
cv = Canvas()
H = 47
cv.ring(230, 232, 'GRAY')
for k in range(36):
    a = k * 10 - H
    if k % 3 == 0:
        cv.radial_bar(a, 202, 226, 4, 'WHITE')
    else:
        cv.radial_bar(a, 216, 226, 2, 'GRAY')
for k, L in enumerate('NESW'):
    rtext(cv, L, 'shapiro', 40, 'ORANGE' if L == 'N' else 'WHITE', k * 90 - H, 172)
icon(cv, 217, 102, 'BLUE', '00100 01110 10101 00100 00100')
cv.text('MAGNETIC', 'bold', 16, 'GRAY', cx=233, y=142, name='caption')
cv.rect(135, 186, 331, 277, 'WHITE')
cv.text('047', 'bold', 86, 'BLACK', x=149, cy=233, name='digits')
cv.text('P +05  R -12', 'mono', 23, 'GRAY', cx=233, y=293, name='tilt')
cv.rect(138, 321, 329, 322, 'GRAY')
cv.text('COVER SCREEN TO RECAL', 'mono', 14, 'GRAY', cx=233, y=328, name='hint')
im = cv.save('out/calib-compass.png')
for n, b in cv.log: print(n, b)
ref = Image.open('ref/compass-heading.png').convert('RGB')
side = Image.new('RGB', (932, 466)); side.paste(ref, (0, 0)); side.paste(im, (466, 0)); side.save('out/calib-side.png')
d = np.abs(np.array(ref).astype(int) - np.array(im).astype(int)).sum(axis=2)
print('mean abs diff', d.mean())
