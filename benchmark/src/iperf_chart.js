import QuickChart from "quickchart-js";
import { readFile } from "fs/promises";

const dataset = await iperf_metrics_to_datasets("./data/iperf_metrics.json");
const labels = [];

Object.values(dataset)
  .sort((a, b) => b.data.length - a.data.length)[0]
  .data.forEach((_) => {
    labels.push(`${labels.length + 1}s`);
  });

await generate_chart(
  {
    type: "line",
    data: {
      labels,
      datasets: Object.values(dataset),
    },
    options: {
      title: {
        display: true,
        text: "Throughput (data/second)",
      },
      scales: {
        yAxes: [
          {
            ticks: {
              callback: (bytes) => {
                if (bytes === 0) return "0 Bytes";

                const k = 1024;
                const sizes = [
                  "Bytes",
                  "KB",
                  "MB",
                  "GB",
                  "TB",
                  "PB",
                  "EB",
                  "ZB",
                  "YB",
                ];

                const i = Math.floor(Math.log(bytes) / Math.log(k));

                return (
                  parseFloat((bytes / Math.pow(k, i)).toFixed(2)) +
                  " " +
                  sizes[i]
                );
              },
            },
          },
        ],
      },
    },
  },
  "iperf",
);

async function iperf_metrics_to_datasets(metrics_file) {
  const metrics = JSON.parse(await readFile(metrics_file, "utf8"));
  const sockets = {};

  metrics.start.connected.forEach((connected, i) => {
    sockets[connected.socket] = {
      label: `${i + 1}`,
      data: [],
      backgroundColor: "transparent",
      borderWidth: 1,
    };
  });

  metrics.intervals.forEach((interval) => {
    interval.streams.forEach((stream) => {
      sockets[stream.socket].data.push(stream.bytes);
    });
  });

  return sockets;
}

async function generate_chart(chart_config, filename) {
  try {
    const chart = new QuickChart();

    chart.setWidth(800);
    chart.setHeight(400);
    chart.setConfig(chart_config);

    await chart.toFile(`./charts/${filename}.jpg`);
  } catch (error) {
    console.error("Error generating chart:", error);
  }
}
