import http from "k6/http";

export const options = {
  vus: 1,
  duration: "30s",
  noConnectionReuse: true,
};

export default function () {
  http.get("http://localhost:3000");
}
