+ CamPilot fixture post-processor for Mach3-compatible controllers (millimetres).
+ Authored for this project; no the vendor post-processor is vendored.
+ Covers every directive and every output block of the dialect, the four helical
+ kinds included (M6 D-M6-8).
POST_NAME = "CamPilot fixture Mach3 Arcs (mm) (*.nc)"
FILE_EXTENSION = "nc"
UNITS = "MM"
LINE_ENDING = "[13][10]"
LINE_NUMBER_START = 0
LINE_NUMBER_INCREMENT = 10
LINE_NUMBER_MAXIMUM = 999999
DIRECT_OUTPUT = "Mach|Mach4.Document"
SUBSTITUTE = "({)}"
ROTARY_WRAP_X = "-A"
ROTARY_WRAP_Y = "-A"
RAPID_PLUNGE_TO_STARTZ = NO
PRINT_DIRECT = "YES"

VAR LINE_NUMBER = [N|A|N|1.0]
VAR SPINDLE_SPEED = [S|A|S|1.0]
VAR TOOL_NUMBER = [T|A||1.0]
VAR FEED_RATE = [F|C|F|1.1]
VAR PLUNGE_RATE = [FP|C|F|1.1]
VAR CUT_RATE = [FC|C|F|1.1|0.0166]
VAR X_POSITION = [X|C|X|1.4]
VAR Y_POSITION = [Y|C|Y|1.4]
VAR Z_POSITION = [Z|C|Z|1.4]
VAR X_HOME_POSITION = [XH|A|X|1.4]
VAR Y_HOME_POSITION = [YH|A|Y|1.4]
VAR Z_HOME_POSITION = [ZH|A|Z|1.4]
VAR SAFE_Z_HEIGHT = [SAFEZ|A|Z|1.4]
VAR ARC_CENTRE_I_INC_POSITION = [I|A|I|1.4]
VAR ARC_CENTRE_J_INC_POSITION = [J|A|J|1.4]
VAR ARC_RADIUS = [Radius|A|U|1.4]
VAR WRAP_DIAMETER = [WRAP_DIA|A||1.4]

begin HEADER
"( [TOOLPATH_NAME] )"
"( [POST_NAME] )"
"[N]G00G17G90G21"
"[N]T[T]M06"
"[N][S]M03"

begin INITIAL_RAPID_MOVE
"[N]G00[X][Y]"

begin FIRST_RAPID_MOVE
"[N]G00[X][Y]"

begin RAPID_MOVE
"[N]G00[X][Y]"

begin PLUNGE_MOVE
"[N]G01[Z][F]"

begin FIRST_FEED_MOVE
"[N]G01[X][Y][Z][F]"

begin FEED_MOVE
"[N]G01[X][Y][Z]"

begin RETRACT_MOVE
"[N]G00[Z]"

begin FIRST_CW_ARC_MOVE
"[N]G02[X][Y][I][J][F]"

begin CW_ARC_MOVE
"[N]G02[X][Y][I][J]"

begin FIRST_CCW_ARC_MOVE
"[N]G03[X][Y][I][J][F]"

begin CCW_ARC_MOVE
"[N]G03[X][Y][I][J]"

begin FIRST_CW_HELIX_MOVE
"[N]G02[X][Y][Z][I][J][F]"

begin CW_HELIX_MOVE
"[N]G02[X][Y][Z][I][J]"

begin FIRST_CCW_HELIX_MOVE
"[N]G03[X][Y][Z][I][J][F]"

begin CCW_HELIX_MOVE
"[N]G03[X][Y][Z][I][J]"

begin FEED_RATE_CHANGE
"[N][F]"

begin NEW_SEGMENT
"[N]G00[Z]"

begin TOOLCHANGE
"[N]M05"
"[N]T[T]M06"

begin FOOTER
"[N]G00[ZH]"
"[N]G00[XH][YH]"
"[N]M05"
"[N]M30"
