# Pet artwork

Six transparent 1536x1024 RGBA PNG atlases generated with Codex's built-in
image-generation tool for Mote on 12 September 2026. Each has eight poses in
four columns and two rows. The images are embedded in the executable.

The renderer extracts the main animal component, anchors its feet, and
uses a shared species scale. PNG alpha is retained; the apparently coloured
background in some image viewers is transparent RGB, not a desktop backdrop.

These are generated realistic raster animals, not 3D skeletal models. The
source atlas has three walking poses, plus standing, sitting, sleeping,
anticipation and leap/stretch poses. Procedural animation adds timing,
transitions and restrained breathing. New poses should be added to the atlas
contract together with matching bounds and animation tests.

## Shared generation prompt

Use case: photorealistic-natural.
Asset type: production animation sprite atlas for a Windows desktop pet.
Create ONE transparent PNG sprite sheet with EXACTLY 8 full-body views of the SAME ANIMAL, a 4-column by 2-row grid of equally sized cells. Each cell contains one animal, full body fully inside the cell with at least 8% padding. No grid lines, no text, no contact sheet background, no shadows cast outside the animal. Genuine transparent alpha background. Consistent scale, same soft neutral daylight, same animal proportions, same camera at animal eye height, predominantly SIDE PROFILE FACING RIGHT; all animals facing RIGHT. Anatomically realistic, detailed physical fur/feathers/scales, naturally cute but absolutely NOT cartoon, NO enlarged eyes, no outlines, no plastic or toy. This is like a high-end realistic wildlife game render.
POSE ORDER, reading left to right:
TOP ROW: 1 neutral standing side profile; 2 walking stride with near forefoot forward and far hindfoot forward; 3 walking passing pose with near forefoot under body; 4 opposite walking stride with far forefoot forward.
BOTTOM ROW: 5 sitting/resting alert naturally with head turned only slightly toward viewer; 6 asleep naturally curled/resting closed eyes; 7 crouched preparing small jump (or for tortoise head tucked low); 8 small airborne leap feet lifted (or owl wings partly spread, tortoise neck stretching forward grounded).
Critical: preserve matching animal markings, anatomy, body length and rendering style in all eight cells. Give each cell the same size and align animal feet to same near-bottom baseline. No extra animals or disconnected body parts. Landscape canvas around 1536x1024.

## Animal-specific prompts appended to the shared prompt

- **cat.png**: a young adult silver-grey striped tabby cat with cream muzzle, green eyes of natural feline size, soft short thick fur, long striped tail. Natural cat anatomy, slender rather than chubby. One cat in each cell.
- **dog.png**: a young adult red Shiba Inu dog, realistic thick ginger and cream double coat, naturally proportioned alert ears, curled tail, black nose, warm brown canine eyes. In sleep curled naturally muzzle under tail.
- **rabbit.png**: a small natural brown-and-white Holland lop rabbit with soft velvety fur, floppy ears, anatomically natural dark eyes and hind legs. Walking poses are stages of a rabbit hop; sleep is a compact resting loaf with eyes closed.
- **fox.png**: a real juvenile red fox, rust-orange soft fur, black paws and ear backs, white throat, full bushy white-tipped tail, natural small amber eyes. Long elegant muzzle and slender natural body, no exaggerated features.
- **owl.png**: a real small barn owl with detailed tawny speckled back feathers, white heart-shaped facial disk, anatomically normal dark eyes, small feathered legs and talons. Pose1 perched standing, poses2-4 three little alternating stepping poses with wings folded, pose5 upright resting facing 3/4 right, pose6 asleep head tucked into shoulder feathers, pose7 crouch wings just loosening, pose8 airborne with wings half open but entirely within its cell. No branch, no props.
- **tortoise.png**: a real small Hermann's tortoise with richly detailed golden ochre-and-dark-brown domed shell scutes, textured olive-brown scaled legs and head, natural tiny dark reptile eyes. Low squat true reptile proportions. Poses2-4 three slow walking strides with neck gently extended, pose5 resting alert, pose6 eyes shut and head tucked, pose7 crouched with head withdrawing, pose8 grounded neck stretching forward and one foreleg reaching; NEVER flying or jumping. No plants, no props.
