-- 開発時専用。非機密の検証ブックを新規作成し、Excel の実表示を保存する。
-- 既存ブックを操作しない。出力先は新しいディレクトリを指定する。
on run argv
    set outputDirectory to item 1 of argv
    do shell script "/usr/bin/test ! -e " & quoted form of (outputDirectory & "/input.xlsx")
    do shell script "/usr/bin/test ! -e " & quoted form of (outputDirectory & "/excel.pdf")
    set cases to {{0, "yyyy-mm-dd"}, {1, "yyyy-mm-dd dddd"}, {60, "yyyy-mm-dd dddd"}, {40729, "yyyy-mm-dd"}, {43585, "ggge年m月d日"}, {43586, "ggge年m月d日"}, {32515, "ggge年m月d日"}, {32516, "ggge年m月d日"}, {0.523958333333333, "h:mm"}, {0.523958333333333, "h"}, {0.9999999, "yyyy-mm-dd hh:mm:ss"}, {43586, "[$-411]mmm ddd"}, {43586, "g gg ggg e ee"}, {1, "0.##"}, {2.675, "0.00"}, {1.5, "[h]:mm:ss"}}
    tell application "Microsoft Excel"
        set wb to make new workbook
        set date 1904 of wb to false
        set ws to worksheet 1 of wb
        set name of ws to "表示形式検証"
        set column width of range "A:A" of ws to 12
        set column width of range "B:B" of ws to 40
        set column width of range "C:C" of ws to 40
        set value of range "A1:C1" of ws to {{"番号", "Excel表示", "表示形式"}}
        set report to ""
        repeat with i from 1 to count of cases
            set oneCase to item i of cases
            set rowNumber to i + 1
            set value of range ("A" & rowNumber) of ws to i
            set targetCell to range ("B" & rowNumber) of ws
            set value of targetCell to item 1 of oneCase
            set number format of targetCell to item 2 of oneCase
            set number format of range ("C" & rowNumber) of ws to "@"
            set value of range ("C" & rowNumber) of ws to item 2 of oneCase
            set report to report & i & tab & (string value of targetCell) & linefeed
        end repeat
        log report
        save as ws filename (outputDirectory & "/input.xlsx") file format Excel XML file format
        save as ws filename (outputDirectory & "/excel.pdf") file format PDF file format
        close wb saving no
        return report
    end tell
end run
