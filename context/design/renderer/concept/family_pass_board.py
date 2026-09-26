"""Review boards for family_pass_v1.py. Run from renderer/ after rendering V1."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'renderer/concept/family-pass-v1-out'
FONT='/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf'
S=466;GAP=24;M=54;HEAD=118;ROW_HEAD=52;CAP=58;ROW_GAP=24
font=ImageFont.truetype(FONT,18);small=ImageFont.truetype(FONT,14)
large=ImageFont.truetype(FONT,39);rowfont=ImageFont.truetype(FONT,24)
rows=[
 ('01 / CLOCK K1',[(s.upper().replace('NODATA','NO DATA').replace('NOZONE','NO ZONE'),'K1 PROPOSAL',OUT/f'clock-K1-{s}.png') for s in ('gnss','rtc','manual','stopped','nozone','nodata')]),
 ('02 / SETTINGS',[
  ('INDEX / 1','S1 SELECTED',ROOT/'renderer/concept/settings-g5-out/settings-S1-sparse-scatter-page-1.png'),
  ('INDEX / 2','S1 SELECTED',ROOT/'renderer/concept/settings-g5-out/settings-S1-sparse-scatter-page-2.png'),
  ('OFFSET / 1','D1 PROPOSAL',OUT/'settings-D1-manual-time.png'),
  ('ZONE / 2','D1 PROPOSAL',OUT/'settings-D1-zone.png'),
  ('BRIGHTNESS','D1 PROPOSAL',OUT/'settings-D1-brightness.png'),
  ('TIMEOUT','D1 PROPOSAL',OUT/'settings-D1-timeout.png')]),
 ('03 / DETAILS + COMPASS',[
  ('DEVICE / TOP','D1 PROPOSAL',OUT/'settings-D1-device.png'),
  ('DEVICE / END','D1 PROPOSAL',OUT/'settings-D1-device-end.png'),
  ('CLEAR / DRAG','D1 PROPOSAL',OUT/'settings-D1-clear-drag.png'),
  ('HEADING','C1 SELECTED',ROOT/'renderer/concept/compass-c1-out/compass-C1-heading.png'),
  ('TOP EDGE UP','C1 D1 PROPOSAL',OUT/'compass-C1-top-edge-up.png'),
  ('SWIPE FRAME','C1 D1 PROPOSAL',OUT/'compass-C1-swiping.png')]),
 ('04 / ALWAYS ON',[
  ('LOCAL / 13:07','H2 SELECTED',ROOT/'renderer/concept/glitch-package/aod-H2-1307.png'),
  ('LOCAL / 13:08','H2 SELECTED',ROOT/'renderer/concept/glitch-package/aod-H2-1308.png'),
  ('NO ZONE','H2 D1 PROPOSAL',OUT/'aod-H2-nozone.png'),
  ('STOPPED','H2 D1 PROPOSAL',OUT/'aod-H2-stopped.png'),
  ('NO DATA','H2 D1 PROPOSAL',OUT/'aod-H2-nodata.png'),
  ('FAULT','EXISTING CAPTURE',ROOT/'references/firmware-captures/2026-09-25/startup-fault.png')])]
W=2*M+6*S+5*GAP;H=HEAD+4*(ROW_HEAD+S+CAP)+3*ROW_GAP+55
board=Image.new('RGB',(W,H),(18,20,23));d=ImageDraw.Draw(board)
d.text((M,32),'OCTOWHERE / FAMILY PASS V1',font=large,fill=(215,218,221))
d.text((W-M-980,52),'SELECTED ANCHORS + D1/K1 PROPOSALS  /  466 PX TILES',font=small,fill=(132,140,149))
for ri,(section,items) in enumerate(rows):
 y=HEAD+ri*(ROW_HEAD+S+CAP+ROW_GAP)
 d.text((M,y+5),section,font=rowfont,fill=(192,254,4))
 d.line((M+380,y+20,W-M,y+20),fill=(64,70,77))
 for ci,(name,status,path) in enumerate(items):
  x=M+ci*(S+GAP);sy=y+ROW_HEAD
  im=Image.open(path).convert('RGB')
  assert im.size==(S,S),(path,im.size)
  board.paste(im,(x,sy));d.rectangle((x,sy,x+S-1,sy+S-1),outline=(65,72,78))
  d.text((x,sy+S+10),name,font=font,fill=(215,218,221))
  d.text((x,sy+S+35),status,font=small,fill=(132,140,149))
d.text((M,H-30),'PROPOSAL = VISUAL STUDY, NOT FIRMWARE     SEE README.md FOR BEHAVIOR AND OPEN CHECKS',font=small,fill=(132,140,149))
board.save(OUT/'family-pass-v1-board.png',optimize=True)

# Focused clock comparison, same fixture, showing what K1 changes.
comparisons=[]
for state in ('gnss','nozone','nodata'):
 old={'gnss':'clock4-state-gnss.png','nozone':'clock4-state-nozone.png','nodata':'clock4-state-nodata.png'}[state]
 comparisons.append((state.upper()+' / ROUND 4',ROOT/'renders'/old))
 comparisons.append((state.upper()+' / K1',OUT/f'clock-K1-{state}.png'))
cw=2*M+3*S+2*GAP;ch=HEAD+2*(S+CAP+ROW_HEAD)+ROW_GAP+45
comp=Image.new('RGB',(cw,ch),(18,20,23));cd=ImageDraw.Draw(comp)
cd.text((M,31),'CLOCK / ROUND 4 → K1',font=large,fill=(215,218,221))
for i,(label,path) in enumerate(comparisons):
 col=i//2;row=i%2;x=M+col*(S+GAP);y=HEAD+row*(S+CAP+ROW_HEAD)
 img=Image.open(path).convert('RGB');comp.paste(img,(x,y))
 cd.text((x,y+S+10),label,font=font,fill=(215,218,221))
comp.save(OUT/'clock-K1-comparison.png',optimize=True)

edge_names=('clock-K1-charging','clock-K1-battery-low','clock-K1-battery-unknown',
            'clock-K1-long-zone','settings-S1-always-on')
edge=Image.new('RGB',(5*S,520),(25,25,28));ed=ImageDraw.Draw(edge)
for i,name in enumerate(edge_names):
    edge.paste(Image.open(OUT/f'{name}.png').convert('RGB'),(i*S,0))
    ed.text((i*S+10,482),name,font=small,fill=(215,218,221))
edge.save(OUT/'edge-state-check.png',optimize=True)
print('review boards:',OUT/'family-pass-v1-board.png',OUT/'clock-K1-comparison.png',OUT/'edge-state-check.png')
