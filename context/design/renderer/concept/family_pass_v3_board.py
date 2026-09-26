"""Review board of selected K1, S1, C1 and proposed D2/H2b concepts."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'renderer/concept/family-pass-v3-out'
V1=ROOT/'renderer/concept/family-pass-v1-out'
V2=ROOT/'renderer/concept/family-pass-v2-out'
F='/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'
S=466;G=22;M=52;N=6;HEAD=108;ROW=47;CAP=52;BETWEEN=22
font=ImageFont.truetype(F,16);small=ImageFont.truetype(F,13)
big=ImageFont.truetype(F,36);section=ImageFont.truetype(F,22)
rows=[
 ('01 / CLOCK K1 — CHOSEN DIRECTION',[(s.upper(),'K1 SELECTED',V1/f'clock-K1-{s}.png') for s in ('gnss','rtc','manual','stopped','nozone','nodata')]),
 ('02 / SETTINGS — CENTERED / VIOLET',[
  ('INDEX 1','S1 SELECTED',ROOT/'renderer/concept/settings-g5-out/settings-S1-sparse-scatter-page-1.png'),
  ('INDEX 2','S1 SELECTED',ROOT/'renderer/concept/settings-g5-out/settings-S1-sparse-scatter-page-2.png'),
  ('OFFSET','D3 PROPOSAL',OUT/'settings-D3-offset.png'),('ZONE','D3 PROPOSAL',OUT/'settings-D3-zone.png'),
  ('BRIGHTNESS','D3 PROPOSAL',OUT/'settings-D3-brightness.png'),('TIMEOUT','D3 PROPOSAL',OUT/'settings-D3-timeout.png')]),
 ('03 / DEVICE — REPLAY ROUTE',[
  ('DEVICE TOP','D3 PROPOSAL',OUT/'settings-D3-device-top.png'),('DEVICE END','D3 PROPOSAL',OUT/'settings-D3-device-end.png'),
  ('REPLAY CHOICE','D3 PROPOSAL',OUT/'settings-D3-replay-choice.png'),('FAILURE DEMO','D3 PROPOSAL',OUT/'settings-D3-replay-failure.png'),
  ('CLEAR DRAG','D1 PROPOSAL',V1/'settings-D1-clear-drag.png'),
  ('COMPASS','C1 SELECTED',ROOT/'renderer/concept/compass-c1-out/compass-C1-heading.png')]),
 ('04 / ALWAYS ON — BATTERY AND UTC',[
  ('LOCAL 13:07','H2b PROPOSAL',V2/'aod-H2b-local-1307.png'),('LOCAL 13:08','H2b PROPOSAL',V2/'aod-H2b-local-1308.png'),
  ('NO ZONE / UTC','H2b PROPOSAL',V2/'aod-H2b-nozone.png'),('STOPPED','H2b PROPOSAL',V2/'aod-H2b-stopped.png'),
  ('NO DATA','H2b PROPOSAL',V2/'aod-H2b-nodata.png'),('LOW BATTERY','H2b PROPOSAL',V2/'aod-H2b-battery-low.png')])]
W=2*M+N*S+(N-1)*G;H=HEAD+len(rows)*(ROW+S+CAP)+(len(rows)-1)*BETWEEN+42
board=Image.new('RGB',(W,H),(18,20,23));d=ImageDraw.Draw(board)
d.text((M,28),'OCTOWHERE / FAMILY PASS V3',font=big,fill=(221,225,231))
d.text((W-M-925,51),'SELECTED ANCHORS AND NEW PROPOSALS / 466 PX TILES',font=small,fill=(132,142,154))
for ri,(label,items) in enumerate(rows):
    y=HEAD+ri*(ROW+S+CAP+BETWEEN)
    d.text((M,y+4),label,font=section,fill=(192,254,4))
    d.line((M+580,y+21,W-M,y+21),fill=(63,70,78))
    for ci,(name,status,path) in enumerate(items):
        x=M+ci*(S+G);sy=y+ROW
        img=Image.open(path).convert('RGB');assert img.size==(S,S),(path,img.size)
        board.paste(img,(x,sy));d.rectangle((x,sy,x+S-1,sy+S-1),outline=(63,70,78))
        d.text((x,sy+S+9),name,font=font,fill=(221,225,231))
        d.text((x,sy+S+32),status,font=small,fill=(132,142,154))
d.text((M,H-27),'READ README.md FOR BEHAVIOR, PROVENANCE AND OPEN VALIDATION',font=small,fill=(132,142,154))
board.save(OUT/'family-pass-v3-board.png',optimize=True)

def comparison(filename,title,columns):
    w=2*M+len(columns)*S+(len(columns)-1)*G;h=HEAD+S+CAP+28
    canvas=Image.new('RGB',(w,h),(18,20,23));dr=ImageDraw.Draw(canvas)
    dr.text((M,30),title,font=big,fill=(221,225,231))
    for i,(name,path) in enumerate(columns):
        x=M+i*(S+G);canvas.paste(Image.open(path).convert('RGB'),(x,HEAD))
        dr.text((x,HEAD+S+13),name,font=font,fill=(221,225,231))
    canvas.save(OUT/filename,optimize=True)
comparison('settings-D3-comparison.png','SETTINGS / D2 → D3',[
    ('OFFSET D2',V2/'settings-D2-offset.png'),('OFFSET D3',OUT/'settings-D3-offset.png'),
    ('ZONE D2',V2/'settings-D2-zone.png'),('ZONE D3',OUT/'settings-D3-zone.png'),
    ('BRIGHTNESS D2',V2/'settings-D2-brightness.png'),('BRIGHTNESS D3',OUT/'settings-D3-brightness.png')])
comparison('aod-H2b-reference.png','AOD / H2 → H2b',[
    ('LOCAL H2',ROOT/'renderer/concept/glitch-package/aod-H2-1307.png'),('LOCAL H2b',V2/'aod-H2b-local-1307.png'),
    ('NO ZONE H2 D1',V1/'aod-H2-nozone.png'),('NO ZONE H2b',V2/'aod-H2b-nozone.png')])
print('rendered boards in',OUT)
