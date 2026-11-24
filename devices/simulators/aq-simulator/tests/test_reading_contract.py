"""The batch this simulator POSTs is hand-built here and hand-parsed by
telemetry. `schemas/fixtures/ingest-batch.json` is the only thing holding the two
to one shape, so a rename here has to fail here rather than stopping readings silently.
"""

import json
from pathlib import Path

from scenarios import Scenario
from simulator import SimulationRoom

_FIXTURE = json.loads(
    (Path(__file__).resolve().parents[4] / "schemas" / "fixtures" / "ingest-batch.json").read_text()
)


def _pinned_readings() -> list[dict]:
    case = next(c for c in _FIXTURE["cases"] if c["producer"] == "aq-simulator")
    return case["body"]["readings"]


def _read_one_tick() -> list[dict]:
    room = SimulationRoom(
        building_id="bldg-3f2b4c5d",
        room_id="room-lab-2",
        scenario=Scenario.OCCUPIED_OFFICE,
    )
    return [reading.model_dump() for reading in room.read(0.1)]


def test_a_tick_sends_one_reading_per_metric_the_fixture_pins():
    assert [r["type"] for r in _read_one_tick()] == [r["type"] for r in _pinned_readings()]


def test_every_reading_carries_exactly_the_fields_the_fixture_pins():
    for reading, pinned in zip(_read_one_tick(), _pinned_readings()):
        assert sorted(reading) == sorted(pinned)


def test_the_reading_does_not_repeat_the_batchs_building():
    """`buildingId` belongs to the batch. Telemetry stamps it onto every reading and
    rejects one naming a different building, so sending it here can only agree or break.
    """
    assert all("buildingId" not in reading for reading in _read_one_tick())
