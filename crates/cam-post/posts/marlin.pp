+ Marlin post-processor (millimetres).
+ Authored for this project; not vendored from any third-party source.
POST_NAME = "Marlin (mm) (*.gcode)"
FILE_EXTENSION = "gcode"
UNITS = "MM"
LINE_ENDING = "[10]"

VAR SPINDLE_SPEED = [S|A|S|1.0]
VAR FEED_RATE = [F|C|F|1.1]
VAR PLUNGE_RATE = [FP|C|F|1.1]
VAR X_POSITION = [X|C|X|1.3]
VAR Y_POSITION = [Y|C|Y|1.3]
VAR Z_POSITION = [Z|C|Z|1.3]
VAR X_HOME_POSITION = [XH|A|X|1.3]
VAR Y_HOME_POSITION = [YH|A|Y|1.3]
VAR Z_HOME_POSITION = [ZH|A|Z|1.3]
VAR ARC_CENTRE_I_INC_POSITION = [I|A|I|1.3]
VAR ARC_CENTRE_J_INC_POSITION = [J|A|J|1.3]

begin HEADER
"G21 G90 G94 G17"
"M3 [S]"

begin FOOTER
"M5"
"G0 X0 Y0"
"M30"

begin INITIAL_RAPID_MOVE
"G0[X][Y][Z]"

begin FIRST_RAPID_MOVE
"G0[X][Y][Z]"

begin RAPID_MOVE
"G0[X][Y][Z]"

begin PLUNGE_MOVE
"G1[Z][FP]"

begin FIRST_FEED_MOVE
"G1[X][Y][Z][F]"

begin FEED_MOVE
"G1[X][Y][Z][F]"

begin RETRACT_MOVE
"G0[Z]"

begin FIRST_CW_ARC_MOVE
"G2[X][Y][I][J][F]"

begin CW_ARC_MOVE
"G2[X][Y][I][J][F]"

begin FIRST_CCW_ARC_MOVE
"G3[X][Y][I][J][F]"

begin CCW_ARC_MOVE
"G3[X][Y][I][J][F]"

begin NEW_SEGMENT
"(new segment)"

begin TOOLCHANGE
"M5"
"(tool change)"
"M3 [S]"
