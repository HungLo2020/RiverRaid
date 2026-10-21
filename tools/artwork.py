#!/usr/bin/env python3
"""Create the original procedural launcher cover; no game ROM is required."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
SIZE = (900, 1200)


def main():
    image = Image.new("RGB", SIZE, "#111d28")
    draw = ImageDraw.Draw(image)
    # The river narrows toward the horizon, with angular banks recalling the game.
    draw.polygon([(0, 330), (900, 330), (900, 1200), (0, 1200)], fill="#3a5725")
    draw.polygon([(0, 410), (300, 330), (430, 550), (260, 780), (100, 1200), (0, 1200)], fill="#5d9518")
    draw.polygon([(900, 370), (620, 330), (480, 620), (690, 820), (840, 1200), (900, 1200)], fill="#5d9518")
    river = [(383, 330), (515, 330), (535, 450), (580, 560), (500, 690), (590, 820), (675, 1200), (175, 1200), (290, 825), (390, 675), (350, 555), (410, 440)]
    draw.polygon(river, fill="#334dbb")
    draw.line([(390, 335), (417, 440), (359, 556), (400, 675), (302, 827), (190, 1200)], fill="#7396e7", width=5)
    draw.polygon([(433, 500), (468, 500), (494, 575), (460, 635), (424, 580)], fill="#8bb13a")
    # Road and bridge across the river, with the original gold center stripe.
    draw.polygon([(0, 710), (900, 650), (900, 698), (0, 758)], fill="#9a9a9a")
    draw.line([(0, 737), (900, 677)], fill="#f6e577", width=5)
    draw.line([(355, 702), (535, 690)], fill="#202c3a", width=8)
    draw.line([(350, 752), (549, 739)], fill="#202c3a", width=8)
    # An original geometric fighter, with a long contrail and pixellike facets.
    draw.polygon([(431, 1155), (455, 910), (479, 1155)], fill="#e7a34a")
    draw.polygon([(450, 800), (478, 916), (562, 989), (556, 1010), (476, 985), (477, 1049), (506, 1085), (450, 1073), (394, 1085), (423, 1049), (424, 985), (344, 1010), (338, 989), (422, 916)], fill="#f6e577")
    draw.polygon([(450, 813), (456, 953), (450, 990), (440, 957)], fill="#ffffff")
    draw.polygon([(450, 870), (459, 934), (442, 934)], fill="#111d28")
    draw.polygon([(590, 426), (617, 426), (626, 432), (621, 442), (585, 442), (574, 435)], fill="#d77a99")
    draw.rectangle((595, 408, 604, 429), fill="#a4d965")
    draw.line((553, 408, 642, 408), fill="#a4d965", width=6)
    draw.polygon([(259, 593), (320, 593), (304, 609), (268, 609)], fill="#6f1200")
    draw.rectangle((282, 576, 297, 592), fill="#a4d965")
    # Strong, legible type remains readable in launcher thumbnails.
    bold = "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf"
    regular = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    title = ImageFont.truetype(bold, 132)
    small = ImageFont.truetype(regular, 25)
    draw.text((450, 58), "RIVER", font=title, anchor="mt", fill="#f6e577", stroke_width=1)
    draw.text((450, 182), "RAID", font=title, anchor="mt", fill="#f6e577", stroke_width=1)
    draw.line((75, 329, 825, 329), fill="#f6e577", width=3)
    draw.rectangle((0, 1140, 900, 1200), fill="#111d28")
    draw.text((450, 1157), "FLY LOW. KEEP MOVING.", font=small, anchor="mt", fill="#f6e577")
    target = ROOT / "assets" / "cover.png"
    target.parent.mkdir(parents=True, exist_ok=True)
    image.convert("RGB").save(target, optimize=True)
    print(target)


if __name__ == "__main__":
    main()
