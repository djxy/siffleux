import QuickChart from "quickchart-js";
import { readFile } from "fs/promises";

const datasets = await Promise.all([
  pidstat_metrics_to_datasets(
    "./data/pidstat_metrics.json",
    "siffleux",
    "rgba(255, 39, 65, 1)",
  ),
  // pidstat_metrics_to_datasets(
  //   "./data/pidstat_metrics copy.json",
  //   "copy",
  //   "rgba(54, 162, 235, 1)",
  // ),
]);
const labels = [];

datasets
  .sort((a, b) => b.cpu.data.length - a.cpu.data.length)[0]
  .cpu.data.forEach((_) => {
    labels.push(`${labels.length + 1}s`);
  });

await generate_chart(
  {
    type: "line",
    data: {
      labels,
      datasets: datasets.map((d) => d.cpu),
    },
    options: {
      title: {
        display: true,
        text: "CPU Usage",
      },
      scales: {
        yAxes: [
          {
            ticks: {
              callback: (val) => `${val}%`,
            },
          },
        ],
      },
    },
  },
  "cpu",
);

await generate_chart(
  {
    type: "line",
    data: {
      labels,
      datasets: datasets.map((d) => d.memory),
    },
    options: {
      title: {
        display: true,
        text: "Memory Usage",
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
  "memory",
);

async function pidstat_metrics_to_datasets(
  pidstat_metrics_file,
  name,
  borderColor,
) {
  const pidstat_metrics = JSON.parse(
    await readFile(pidstat_metrics_file, "utf8"),
  );
  const statistics = pidstat_metrics.sysstat.hosts[0].statistics;
  const cpu_datasets = [];
  const memory_datasets = [];

  statistics.forEach((s) => {
    cpu_datasets.push(s["task-cpu-load"][0].cpu);
    memory_datasets.push(s["task-memory"][0].RSS * 1024);
  });

  return {
    cpu: {
      label: name,
      data: cpu_datasets,
      borderColor,
      backgroundColor: "transparent",
      borderWidth: 1,
    },
    memory: {
      label: name,
      data: memory_datasets,
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
