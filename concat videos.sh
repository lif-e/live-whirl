for f in $(ls -1v ./*.mov); do
	ffmpeg -ss 0.01667 -i $f -c copy "./trimmed/${f}"
	echo "file './trimmed/$f'" >> mylist.txt;
done
ffmpeg -f concat -safe 0 -i mylist.txt -c copy ./concated.mov
ffmpeg -i ./concated.mov -vcodec libx264 -crf 32 ./reencoded.mov