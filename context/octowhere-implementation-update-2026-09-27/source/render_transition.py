"""Review renders for the solid/segmented clock battery transition.

The segment layer is drawn above a full-height solid layer. The solid layer's
top edge travels down on charge entry and up on interruption. Fixture timing
is an illustration for implementation review, not a firmware commitment.
"""
from pathlib import Path
from PIL import Image, ImageDraw
import render_fill as R

HERE = Path(__file__).resolve().parent
FRAME_MS = 30


def transition(state='gnss', pct=87, entering=True, progress=0.0, phase=27):
    """Return a clock frame; progress 0 is source, 1 is destination."""
    if pct is None or pct <= 0:
        return R.compose(state, pct=pct, charging=entering, phase=phase)
    h = round(99 * min(100, max(0, pct)) / 100)
    top = 308 - h
    # The presence of CHG follows the charging signal at the first frame.
    im = R.compose(state, pct=pct, charging=entering, phase=phase)
    segments = R.compose(state, pct=pct, charging=True, phase=phase)
    color = R.gauge_color(state, pct)
    # Smoothstep gives zero velocity at both ends. The intact material
    # retreats top-to-bottom on entry and rises bottom-to-top on exit.
    eased = R.ease(progress)
    exposed = eased if entering else 1 - eased
    solid_top = top + round(h * exposed)
    d = ImageDraw.Draw(im)
    d.rectangle((397, top, 436, 307), fill=R.BLACK)
    if solid_top <= 307:
        d.rectangle((397, solid_top, 436, 307), fill=color)
    # Paste only the colored segment pixels, retaining the lower solid layer
    # through the gap regions until its edge has passed them.
    crop = segments.crop((397, top, 437, 308))
    mask = Image.new('L', crop.size, 0)
    mask.putdata([255 if pixel == color else 0 for pixel in crop.get_flattened_data()])
    im.paste(crop, (397, top), mask)
    return im


def frames(state='gnss', pct=87, entering=True):
    count = 16 if pct >= 35 else 7  # final key at 450 ms or 180 ms
    return [transition(state, pct, entering, i/(count-1)) for i in range(count)]


def save_gif(images, path):
    images[0].save(HERE/path, save_all=True, append_images=images[1:],
                   duration=[FRAME_MS]*len(images), loop=0,
                   optimize=True, disposal=2)


def storyboard():
    columns = 5
    states = [('gnss',87), ('gnss',12), ('stopped',87),
              ('nozone',87), ('nodata',87)]
    progress = [0, .25, .5, .75, 1]
    labels = ['SOURCE', '25%', '50%', '75%', 'DESTINATION']
    crop = (389, 199, 445, 316)
    zoom = 3
    tile_w, tile_h = 194, 412
    out = Image.new('RGB',(columns*tile_w+20, len(states)*2*tile_h+20),R.PANEL)
    d = ImageDraw.Draw(out)
    for row,(state,pct) in enumerate(states):
        for direction in (True,False):
            y=20+(row*2+(0 if direction else 1))*tile_h
            for c,p in enumerate(progress):
                x=20+c*tile_w
                frame=transition(state,pct,direction,p)
                out.paste(frame.crop(crop).resize((168,351),Image.Resampling.NEAREST),(x,y))
                caption=f'{R.LABELS[state]} {pct}%'
                d.text((x,y+357),caption,font=R.small,fill=R.INK)
                d.text((x,y+380),('IN ' if direction else 'OUT ')+labels[c],font=R.small,fill=R.INK)
    out.save(HERE/'clock-charge-transition-storyboard.png',optimize=True)


def clock_board():
    items=[]
    for state,pct in [('gnss',87),('gnss',12),('stopped',87),('nodata',87)]:
        for direction in (True,False):
            for progress in (0,.5,1):
                label=f'{R.LABELS[state]} {pct}% / '+('START' if direction else 'STOP')+f' {progress:.0%}'
                items.append((label,transition(state,pct,direction,progress)))
    R.board(items,3,'clock-charge-transition-full-screens.png')


def verify():
    well=(394,206,440,311)
    for state,pct in [('gnss',87),('gnss',12),('stopped',87),('nozone',87),('nodata',87)]:
        for direction in (True,False):
            source=R.compose(state,pct,charging=not direction)
            dest=R.compose(state,pct,charging=direction)
            # CHG text changes on the charging signal, so endpoint comparison
            # is confined to the gauge while checking the text separately.
            assert transition(state,pct,direction,0).crop(well).tobytes()==source.crop(well).tobytes()
            assert transition(state,pct,direction,1).crop(well).tobytes()==dest.crop(well).tobytes()
            for i in range(11):
                frame=transition(state,pct,direction,i/10)
                assert frame.crop((0,0,260,466)).tobytes()==dest.crop((0,0,260,466)).tobytes()
    for state in R.STATES:
        solid=R.compose(state,charging=False)
        dashes=R.compose(state,pct=None,charging=False)
        assert solid.getpixel((400,250))==R.gauge_color(state,87)
        assert dashes.getpixel((407,256))==R.GRAY


def main():
    verify()
    R.main()
    save_gif(frames(), 'clock-charge-entry.gif')
    save_gif(frames(entering=False), 'clock-charge-interruption.gif')
    save_gif(frames(pct=12), 'clock-charge-entry-low.gif')
    save_gif(frames(pct=12,entering=False), 'clock-charge-interruption-low.gif')
    storyboard()
    clock_board()
    print('Rendered and verified battery transition study to',HERE)


if __name__ == '__main__':
    main()
