import QuickChart from "quickchart-js";
import { readFile } from "fs/promises";

const datasets = await Promise.all([
  vegeta_metrics_to_datasets(
    "./data/vegeta_metrics.json",
    "siffleux",
    "rgba(255, 39, 65, 1)",
  ),
]);
const labels = [];

datasets
  .sort((a, b) => b.latency_p50.data.length - a.latency_p50.data.length)[0]
  .latency_p50.data.forEach((_) => {
    labels.push(`${labels.length}s`);
  });

await generate_chart(
  {
    type: "line",
    data: {
      labels,
      datasets: datasets.flatMap((d) => [d.latency_p50, d.latency_p99]),
    },
    options: {
      title: {
        display: true,
        text: "Latency",
      },
      scales: {
        yAxes: [
          {
            ticks: {
              callback: (val) => `${val.toFixed(2)}ms`,
            },
          },
        ],
      },
    },
  },
  "vegeta_latency",
);

await generate_chart(
  {
    type: "line",
    data: {
      labels,
      datasets: datasets.map((d) => d.throughput),
    },
    options: {
      title: {
        display: true,
        text: "Throughput (requests/second)",
      },
      scales: {
        yAxes: [
          {
            ticks: {
              callback: (val) => `${val} req/s`,
            },
          },
        ],
      },
    },
  },
  "vegeta_throughput",
);

async function vegeta_metrics_to_datasets(
  vegeta_metrics_file,
  name,
  borderColor,
) {
  const vegeta_metrics = (await readFile(vegeta_metrics_file, "utf8"))
    .trim()
    .split("\n");
  const latency_50_datasets = [];
  const latency_99_datasets = [];
  const throughput_datasets = [];

  vegeta_metrics.forEach((line) => {
    if (!line) return;
    const metrics = JSON.parse(line);

    throughput_datasets.push(metrics.throughput);
    latency_50_datasets.push(
      parseFloat((metrics.latencies["50th"] / 1000000).toFixed(2)),
    );
    latency_99_datasets.push(
      parseFloat((metrics.latencies["99th"] / 1000000).toFixed(2)),
    );
  });

  return {
    latency_p50: {
      label: `${name} - p50`,
      data: latency_50_datasets,
      borderColor,
      backgroundColor: "transparent",
      borderWidth: 1,
    },
    latency_p99: {
      label: `${name} - p99`,
      data: latency_99_datasets,
      borderColor,
      backgroundColor: "transparent",
      borderWidth: 1,
    },
    throughput: {
      label: name,
      data: throughput_datasets,
      borderColor,
      backgroundColor: "transparent",
      borderWidth: 1,
    },
  };
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
