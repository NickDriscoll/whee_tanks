import re, os

filenames = ["albedo", "normal", "roughness", "height", "metallic"]

for entry in os.scandir("."):
	if entry.is_dir():
		for e in os.scandir(entry.path):
			for fname in filenames:
				if re.search(fname, e.name):
					os.rename(e.path, entry.path + "/" + fname + ".png")
					break