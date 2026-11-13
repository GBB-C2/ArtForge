"""Exercise the real FFmpeg filter and image sequence contract, without Rust/GPU.
Run: python3 -m unittest discover -s scripts -p 'test_video_import.py'
FFmpeg is optional for this host-side check. Source conversion is local only.
"""
from pathlib import Path
import re, shutil, subprocess, tempfile, unittest

ROOT=Path(__file__).resolve().parents[1]
FILTER=re.search(r'pub const VIDEO_FILTER: &str = "([^"]+)";', (ROOT/'crates/io/src/video.rs').read_text()).group(1)
FFMPEG=shutil.which('ffmpeg')

@unittest.skipUnless(FFMPEG, 'FFmpeg unavailable')
class VideoImportTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder=Path(self.temp.name)
    def run_ffmpeg(self,*args,input=None):
        return subprocess.run([FFMPEG,'-nostdin','-hide_banner','-loglevel','error',*map(str,args)],input=input,capture_output=True,timeout=45,check=True)
    def source(self,width,height,frames=4,sar=None):
        path=self.folder/'клип с пробелами.mkv'
        filters=f'color=c=red:s={width}x{height}:r=24'
        if sar:filters+=f',setsar={sar}'
        self.run_ffmpeg('-f','lavfi','-i',filters,'-frames:v',frames,'-c:v','ffv1',path)
        return path
    def convert(self,path,frames=1441):
        dest=self.folder/'converted';dest.mkdir()
        self.run_ffmpeg('-threads','2','-filter_threads','1','-protocol_whitelist','file,pipe','-i',path,
                        '-map','0:v:0','-an','-sn','-dn','-vf',FILTER,'-frames:v',frames,'-q:v','3','-threads','2','-f','image2',dest/'frame-%06d.jpg')
        return sorted(dest.glob('*.jpg'))
    def test_local_unicode_filename_and_jpeg_frame_count(self):
        from PIL import Image
        frames=self.convert(self.source(32,24))
        self.assertEqual(len(frames),4)
        with Image.open(frames[0]) as image:
            self.assertEqual(image.size,(32,24));r,g,b=image.convert('RGB').getpixel((16,12));self.assertGreater(r,220);self.assertLess(g,20);self.assertLess(b,20)
        for frame in frames:
            data=frame.read_bytes();self.assertTrue(data.startswith(b'\xff\xd8'));self.assertTrue(data.endswith(b'\xff\xd9'))
    def test_portrait_is_bounded_and_keeps_display_aspect(self):
        from PIL import Image
        frames=self.convert(self.source(1080,1920,1))
        with Image.open(frames[0]) as image:self.assertEqual(image.size,(576,1024))
    def test_anamorphic_video_is_normalized_to_square_pixels(self):
        from PIL import Image
        frames=self.convert(self.source(720,576,1,'16/15'))
        with Image.open(frames[0]) as image:self.assertEqual(image.size,(768,576))
    def test_rate_conversion_samples_all_timeline_colors(self):
        from PIL import Image
        path=self.folder/'colors.mkv'
        pixels=b''.join(bytes(c)*16*16 for c in [(255,0,0),(0,255,0),(0,0,255)])
        self.run_ffmpeg('-f','rawvideo','-pixel_format','rgb24','-video_size','16x16','-framerate','12','-i','pipe:0','-frames:v','3','-c:v','ffv1',path,input=pixels)
        frames=self.convert(path);self.assertEqual(len(frames),6)
        dominant=[]
        for frame in frames:
            with Image.open(frame) as image:
                rgb=image.convert('RGB').getpixel((8,8));dominant.append(max(range(3),key=lambda i:rgb[i]))
        self.assertEqual(dominant,[0,0,1,1,2,2])
    def test_long_clip_stops_at_rejection_sentinel_instead_of_silently_truncating(self):
        frames=self.convert(self.source(16,16,1442))
        self.assertEqual(len(frames),1441)

if __name__=='__main__':unittest.main()
