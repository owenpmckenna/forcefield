python -m http.server &
PIDNUM=$!
sleep 1
#echo Just backgrounded the http server as PID $PIDNUM
sudo docker exec -it citadel sh -c "curl 192.168.0.31:8000/connection_file"
kill $PIDNUM
