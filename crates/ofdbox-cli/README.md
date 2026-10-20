# ofdbox CLI

批量转换 OFD、PDF、PNG。

```powershell
cargo run -p ofdbox-cli -- convert <输入...> -t <pdf|ofd|png> -o <输出目录>
```

## 参数

- `inputs`：文件或目录，可多个；目录默认递归。
- `-t, --to`：`pdf`、`ofd`、`png`。
- `-f, --from`：`ofd` 或 `pdf`；不填按扩展名识别。
- `-o, --output`：输出目录，默认 `converted`。
- `--pages 1-3,5`：只转换指定页；默认全部。
- `--dpi 144`：PNG 渲染 DPI。
- `--vector`：PDF→OFD 使用实验性矢量模式；默认栅格，兼容性更好。
- `--force`：覆盖已存在文件；默认自动改名。
- `--no-recursive`：目录不递归。
- `--quiet`：只输出错误和汇总。
- `--report <文件>`：写汇总和失败明细。

## 示例

```powershell
# 目录内所有 OFD/PDF 转 PDF
ofdbox convert .\in -t pdf -o .\out

# 指定页转 300 DPI PNG
ofdbox convert .\a.ofd .\b.pdf -t png -o .\png --pages 1-5 --dpi 300

# PDF 转 OFD（默认栅格；矢量加 --vector）
ofdbox convert .\in.pdf -t ofd -o .\out
```

输出命名：

- PDF/OFD：`<输出目录>/<文件名>.pdf|ofd`
- PNG：`<输出目录>/<文件名>/page_0001.png...`

退出码：全部失败为 1，否则为 0。失败文件不会中断其余转换。
